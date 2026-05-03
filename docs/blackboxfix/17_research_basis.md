# Research Basis

This planning set uses current public standards and project patterns as
design input. The goal is not to copy any one framework, but to align
BlackBoxFix with how buyers already think about AI risk, security,
provenance, observability, and tool integration.

## External References

### NIST AI Risk Management Framework

Reference: <https://www.nist.gov/itl/ai-risk-management-framework>

Design implication:

- map BlackBoxFix controls to Govern, Map, Measure, and Manage;
- produce evidence for design, deployment, use, and evaluation of AI systems;
- support risk management instead of only debugging.

### OWASP Top 10 For LLM And GenAI Applications

References:

- <https://owasp.org/www-project-top-10-for-large-language-model-applications/>
- <https://genai.owasp.org/llm-top-10/>

Design implication:

- treat prompt injection, sensitive data disclosure, supply chain, improper
  output handling, excessive agency, and unbounded consumption as first-class
  risk labels;
- build tool authorization and source-of-truth verification around excessive
  agency risks.

### OWASP Authorization And Logging Guidance

References:

- <https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html>
- <https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html>
- <https://owasp.org/Top10/2021/A09_2021-Security_Logging_and_Monitoring_Failures/>

Design implication:

- validate permissions for each governed operation;
- deny by default when no explicit permission grants the action;
- log access-control failures with enough context for investigation;
- Phase 8 hardening persists `enterprise_access_decision` rows for
  `export-audit` authorization attempts before creating any export files, so
  allowed and denied export attempts are both inspectable in the append-only
  ledger.

### MITRE ATLAS

Reference: <https://atlas.mitre.org/>

Design implication:

- classify security incidents using an adversarial AI taxonomy;
- give CISOs and SOC teams familiar threat labels;
- connect agent incidents to real-world attack patterns when applicable.

### OpenTelemetry

Reference: <https://opentelemetry.io/docs/what-is-opentelemetry/>

Design implication:

- export traces, metrics, and logs to existing observability stacks (reserved
  after Phase 11);
- keep the BlackBoxFix ledger as the authoritative evidence source;
- use semantic attributes for run, step, tool, verifier, connector, risk tier.

### W3C PROV

References:

- <https://www.w3.org/TR/prov-overview/>
- <https://www.w3.org/TR/prov-dm/>

Design implication:

- model provenance with entities, activities, and agents;
- represent tool calls as activities and source-of-truth records as entities;
- preserve responsibility and derivation across runs.
- Phase 4 evidence packets include a W3C-PROV-inspired `provenance` object
  and physical ledger row references so quality/reliability assessment is
  grounded in durable records, not CLI return values.

### OpenLineage

Reference: <https://openlineage.io/docs/>

Design implication:

- borrow the run/job/dataset lineage pattern for data workflows;
- use facets or facet-like extensions for verifier evidence;
- make lineage export possible for data-agent workflows.
- Phase 5 manifests separate static source-ledge metadata from per-row episode
  data, mirroring the run/job/dataset distinction rather than flattening all
  lineage into one row.

### JSON Lines

Reference: <https://jsonlines.org/>

Design implication:

- export one complete UTF-8 JSON value per line so rows can be streamed,
  concatenated, and validated independently;
- reject blank lines and invalid per-line JSON during readback;
- include a line terminator after each row to make future concatenation safe.
- Phase 5 implements this by writing `episodes.jsonl`, re-reading every line,
  validating required fields, and hashing the exact bytes written.

### in-toto

References:

- <https://in-toto.io/>
- <https://github.com/in-toto/attestation/tree/main/spec>

Design implication:

- use supply-chain style metadata for who did what, in what order;
- make action chains transparent;
- consider signed attestations for evidence packets (Phase 4/5).
- Phase 4 keeps evidence packets structured as explicit subject-like record
  references plus predicate-like verifier details, leaving room for a future
  signed envelope without changing the ledger's proof model.

### SLSA Provenance

References:

- <https://slsa.dev/spec/v1.2/provenance>
- <https://slsa.dev/spec/v1.1/provenance>

Design implication:

- describe how artifacts were produced;
- let consumers verify that production matched expectations;
- apply provenance thinking to agent-produced code, config, and reports.
- Phase 4 replay/export follows the same direction: capture the invocation,
  source inputs, resolved source-of-truth readbacks, byproduct evidence, and
  verifier decision so later consumers can inspect how a claim was produced.

### Model Context Protocol

References:

- <https://modelcontextprotocol.io/specification/2025-06-18>
- <https://modelcontextprotocol.io/specification/2025-06-18/server/tools>
- <https://modelcontextprotocol.io/specification/2025-03-26/basic/transports>
- <https://www.jsonrpc.org/specification>
- <https://owasp.org/www-project-mcp-top-10/>

Design implication:

- support MCP as a standard way for agents to access tools and context
  (Phase 6);
- use newline-delimited JSON-RPC stdio for subprocess MCP servers;
- expose verified tool wrappers through MCP with deterministic tool schemas;
- return tool execution failures as structured MCP tool results with
  `isError=true`, while preserving standard JSON-RPC protocol errors for
  malformed protocol requests;
- enforce least-privilege capability scope and authorization for MCP tools;
- treat tool descriptions as untrusted unless from trusted servers.

### OpenID Connect Core

Reference: <https://openid.net/specs/openid-connect-core-1_0-18.html>

Design implication:

- OIDC tenant config must bind to explicit issuer and client id metadata;
- ID-token validation needs required `iss`, `sub`, `aud`, and `exp` claims;
- Phase 8 persists only metadata and SHA-256 fingerprints, not tokens or
  client secrets.

### SAML 2.0 Core

Reference: <https://docs.oasis-open.org/security/saml/v2.0/saml-core-2.0-os.pdf>

Design implication:

- SAML tenant config must bind to explicit entity id, SSO URL, audience, and
  signing-certificate fingerprint;
- Phase 8 refuses raw certificate/key material and stores only SHA-256
  fingerprints needed for security review.

### SCIM Protocol

Reference: <https://www.rfc-editor.org/rfc/rfc7644>

Design implication:

- directory snapshots are modeled as SCIM User and Group state;
- user/group references and group-to-role assignments are validated before
  persistence;
- malformed directory snapshots fail before writing ledger rows.

### NIST RBAC

Reference: <https://csrc.nist.gov/projects/role-based-access-control>

Design implication:

- permissions are assigned through roles, and principals inherit roles through
  directory groups;
- allowed and denied decisions persist as explicit audit rows.

### Computer Security Log Management

References:

- <https://csrc.nist.gov/pubs/sp/800/92/final>
- <https://www.rfc-editor.org/rfc/rfc5424>

Design implication:

- audit export must support security review and SIEM ingestion;
- Phase 8 writes both JSONL audit events and RFC5424-style SIEM lines, then
  reads both files back and verifies hashes before reporting success.

### SQLite WAL And Triggers

References:

- <https://www.sqlite.org/wal.html>
- <https://www.sqlite.org/lang_createtrigger.html>

Design implication:

- WAL journaling + `synchronous=FULL` is enough for a tamper-evident local
  ledger when combined with `BEFORE UPDATE`/`BEFORE DELETE` `RAISE(ABORT)`
  triggers; Phase 1 uses exactly this pattern.

### Git Plumbing And Scriptable Readback

References:

- <https://git-scm.com/docs/git-rev-parse/2.45.3>
- <https://git-scm.com/docs/git-status>
- <https://git-scm.com/docs/git-diff>
- <https://git-scm.com/docs/git-cat-file>

Design implication:

- use `git rev-parse --verify --quiet --end-of-options <rev>^{commit}` for
  branch/commit existence checks against untrusted verifier input;
- use `git status --porcelain=v1 --untracked-files=all` for script-stable
  working-tree cleanliness;
- use `git diff --no-ext-diff --no-color -- <path>` for exact patch readback;
- use `git cat-file -t -- <object>` to record object type evidence.

### Rust Process Execution

Reference: <https://doc.rust-lang.org/std/process/struct.Command.html>

Design implication:

- pass command argv as discrete strings through `std::process::Command`, not a
  shell string;
- invalid command schema must block before execution;
- stdout/stderr/exit code are evidence, while artifact files must still be
  read and hashed from disk.

### GitHub REST API And GitHub Actions

References:

- <https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api?apiVersion=2022-11-28>
- <https://docs.github.com/en/rest/pulls/pulls?apiVersion=2022-11-28>
- <https://docs.github.com/en/rest/issues/issues?apiVersion=2022-11-28>
- <https://docs.github.com/en/rest/issues/comments?apiVersion=2022-11-28>
- <https://docs.github.com/en/rest/using-the-rest-api/using-pagination-in-the-rest-api?apiVersion=2022-11-28>
- <https://docs.github.com/en/rest/commits/commits?apiVersion=2022-11-28>
- <https://docs.github.com/en/rest/checks/suites?apiVersion=2022-11-28>
- <https://docs.github.com/en/rest/actions/workflow-runs?apiVersion=2022-11-28>

Design implication:

- use the documented media type and API version headers for every readback;
- use authenticated requests when private repos or higher rate limits are
  required, but allow unauthenticated reads for public fixtures;
- treat `401`/`403` as permission blocks and rate-limit headers as hard
  blocks, not warnings;
- follow redirects and avoid manually parsing response URLs for identity;
- do not ignore repeated `4xx`/`5xx` errors. Endpoint-specific absent-resource
  responses such as commit/check-suite `422` are captured as verifier failures
  only where the endpoint contract and live readback prove they mean “target
  absent.”
- every pull request is also an issue in GitHub's issue/comment APIs, so
  ticket verifiers must reject issue payloads that contain a `pull_request`
  marker instead of treating a PR body as ticket evidence;
- issue comments are paginated with `per_page` up to 100 and `Link` headers,
  so a ticket evidence verifier must persist comment page evidence and block
  malformed or incomplete pagination rather than silently ignoring comments.

### GitLab REST API, Pipelines, And Jobs

References:

- <https://docs.gitlab.com/api/pipelines/>
- <https://docs.gitlab.com/api/jobs/>
- <https://docs.gitlab.com/api/rest/authentication/>
- <https://docs.gitlab.com/api/rest/#pagination>

Design implication:

- verify pipelines through documented project pipeline endpoints and require
  pipeline id, status, ref, and SHA to match the expected delta;
- verify jobs through documented jobs endpoints and treat all status values as
  a closed set from GitLab docs;
- send optional private/project/group tokens in the documented
  `PRIVATE-TOKEN` header, while allowing public unauthenticated fixtures;
- read paginated jobs completely using GitLab `Link` and `x-next-page`
  pagination headers and record each API page in evidence. Missing, malformed,
  or incomplete pagination is a connector block, not a partial pass;
- store request id, ETag, rate-limit headers, pagination headers, response
  body hash, and normalized observed state in `readback_snapshot` so operators
  can diagnose API/schema/rate-limit failures from the ledger.

### Scanner Artifact Standards

References:

- <https://google.github.io/osv-scanner/output/>
- <https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html>
- <https://cyclonedx.org/docs/1.6/json/>
- <https://github.com/CycloneDX/specification/blob/master/schema/bom-1.6.schema.json>

Design implication:

- OSV-Scanner JSON output is the machine-readable source for vulnerability
  package, ecosystem, version, id, alias, and grouped-id evidence;
- `Cargo.lock` is the durable exact dependency-resolution artifact for Rust,
  so lockfile version checks must parse the TOML package table rather than
  scan prose or command output;
- CycloneDX JSON SBOM evidence must require `bomFormat="CycloneDX"`,
  `specVersion`, and component entries before any SBOM verifier can pass;
- Phase 10 reads physical artifact files, stores byte counts and SHA-256
  hashes in `readback_snapshot`, and treats missing files, schema drift, and
  malformed JSON as typed verifier failures/blocks instead of falling back to
  scanner process return values.

## Internal Project References

- `docs/upgradeplancombine/09_fsv_extensions.md` — FSV protocol primitives
  (the broader Context Graph FSV doctrine).
- `docs/upgradeplancombine/28_bundle_training_corpus_exporter.md` — JSONL
  exporter contract using state/action/expected/readback/verifier rows.
- `docs/5090jepa/13_mcp_and_cli_surface.md` — strict MCP/CLI tool surface
  (the surface BlackBoxFix mirrors).
- `docs/5090jepa/16_troubleshooting_and_failure_taxonomy.md` — typed failure
  and remediation discipline.
- `crates/context-graph-cli/src/commands/export_dynamicjepa.rs` — the
  reference Rust pattern for the Phase 5 episode exporter.
- `crates/context-graph-cli/tests/dynamicjepa_bundle_export_fsv.rs` — the
  reference FSV test pattern (real input → real output → physical readback).

## Synthesis

The strongest product shape is:

- OpenTelemetry-compatible, but not limited to telemetry.
- PROV/OpenLineage-inspired, but focused on agent actions.
- in-toto/SLSA-inspired, but generalized beyond software builds.
- NIST/OWASP/MITRE-aligned, but operational enough to generate evidence.
- MCP-compatible, but independent from any one agent framework.

Phase 1 already commits to the foundational primitives: append-only
hash-chained storage, canonical JSON, deterministic redaction, typed errors.
Every higher-level standard above maps onto those primitives without
requiring a new storage backend.
