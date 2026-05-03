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
- Phase 14 release gates turn measured scorecard/evidence/privacy state into
  an explicit governed promote/block decision instead of leaving readiness as
  an informal dashboard interpretation.
- Phase 16 risk assessments turn remaining product/security uncertainty into
  an explicit Govern/Map/Measure/Manage artifact backed by ledger evidence and
  owner/date-bound decisions.

### NIST SP 800-30 Risk Assessments

Reference: <https://csrc.nist.gov/pubs/sp/800/30/r1/final>

Design implication:

- risk assessment output must support a decision about what action to take in
  response to identified risks;
- Phase 16 persists accepted and blocked risk postures as ledger rows instead
  of leaving them as prose, and includes required owner, rationale, evidence,
  review trigger, severity, and due-date fields for each open-question
  decision.

### NIST Secure Software Development Framework

Reference: <https://csrc.nist.gov/pubs/sp/800/218/final>

Design implication:

- software release readiness should be based on explicit verification,
  artifact/evidence review, and traceable practices;
- Phase 14 applies that release-control pattern to agent runs by requiring
  physical ledger evidence, scorecards, RBAC, and incident/privacy checks
  before a run can be approved for promotion.
- Phase 15 applies the same archive-and-provenance pattern to buyer proof:
  package files are hashed in a manifest and tied back to source ledger refs
  instead of being free-floating collateral.
- Phase 16 uses the same root-cause prevention pattern for risk management:
  invalid decisions, missing pilot evidence, unresolved incidents, and
  runtime durability drift block readiness with typed errors or durable
  blocked assessments.
- Phase 18 applies the same traceable verification pattern to the first
  buyer demo: the package is generated only from persisted verifier/evidence
  rows and manifest-hashed files, not from an agent transcript.

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
- Phase 16 applies the logging guidance to risk posture: denied risk reviews,
  invalid decision registers, secret-like decision text, and blocked readiness
  checks surface typed errors with source-of-truth, field path, and
  remediation; no fallback assessment is synthesized.

### MITRE ATLAS

Reference: <https://atlas.mitre.org/>

Design implication:

- classify security incidents using an adversarial AI taxonomy;
- give CISOs and SOC teams familiar threat labels;
- connect agent incidents to real-world attack patterns when applicable.

### OpenTelemetry

References:

- <https://opentelemetry.io/docs/what-is-opentelemetry/>
- <https://opentelemetry.io/docs/specs/otel/protocol/file-exporter/>
- <https://opentelemetry.io/docs/specs/otel/protocol/>
- <https://opentelemetry.io/docs/concepts/signals/traces/>
- <https://opentelemetry.io/docs/concepts/signals/logs/>
- <https://opentelemetry.io/docs/concepts/signals/metrics/>
- <https://opentelemetry.io/docs/specs/semconv/resource/>

Design implication:

- export traces, metrics, and logs to existing observability stacks through
  OTLP JSONL files (Phase 23 shipped);
- keep the BlackBoxFix ledger as the authoritative evidence source;
- use resource attributes for stable service identity and custom
  low-cardinality attributes for ledger/run/tool/verifier dimensions;
- keep high-cardinality values such as record hashes as span/log attributes,
  not metric dimensions.
- Phase 15 package usage dimensions intentionally avoid per-event UUID-style
  dimensions and keep high-cardinality uniqueness in the idempotency
  identifier/source ref, not metric dimensions.
- Phase 23 follows the same cardinality rule: metrics aggregate by record
  kind, verifier status, and policy status; trace/log records carry detailed
  record ids and hashes for forensic drilldown.

### Ticketing API Readback

References:

- Jira Cloud REST API, Get issue:
  <https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/>
- Linear GraphQL API getting started and issue examples:
  <https://linear.app/developers/graphql>
- ServiceNow Table API:
  <https://www.servicenow.com/docs/bundle/yokohama-api-reference/page/integrate/inbound-rest/concept/c_TableAPI.html>

Design implication:

- Phase 22 verifies ticket state from captured provider API artifacts rather
  than agent text or ticketing-tool return values.
- The connector normalizes only provider-documented readback shapes: Jira
  issue/search fields, Linear GraphQL issue fields including workflow state,
  and ServiceNow Table API `result` records/lists.
- Ticket state proof fails closed on missing identities, mismatched status,
  duplicate ticket identities, invalid JSON, schema drift, or missing evidence
  URL so ticketing cannot become an unverifiable governance checkbox.

### Kubernetes Deployment Status And Image Digests

References:

- <https://kubernetes.io/docs/reference/kubectl/generated/kubectl_get/>
- <https://kubernetes.io/docs/concepts/workloads/controllers/deployment/>
- <https://kubernetes.io/docs/concepts/containers/images/>
- <https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.36/>

Design implication:

- Phase 20 uses physical `kubectl get deployments.v1.apps -o json` output as
  the read path so verifier evidence is independent of apply/rollout command
  return values;
- image verification requires OCI `sha256` digests because Kubernetes
  documents tags as movable while digests uniquely identify image content;
- rollout verification reads Deployment `status.conditions`,
  `observedGeneration`, and replica counters instead of treating CLI success
  as proof of cluster state.

### Terraform JSON, State, And Cloud Audit Evidence

References:

- <https://developer.hashicorp.com/terraform/internals/json-format>
- <https://developer.hashicorp.com/terraform/cli/commands/show>
- <https://developer.hashicorp.com/terraform/cli/commands/plan>
- <https://developer.hashicorp.com/terraform/language/state>
- <https://docs.aws.amazon.com/awscloudtrail/latest/userguide/cloudtrail-event-reference-record-contents.html>
- <https://docs.aws.amazon.com/awscloudtrail/latest/userguide/view-cloudtrail-events.html>

Design implication:

- Terraform verification should consume `terraform show -json` artifacts, not
  CLI human output or an apply return value;
- JSON `format_version` major versions must be rejected when unsupported,
  while minor-version additions can be ignored if core fields still parse;
- plan verification needs to inspect `resource_changes` and output changes
  directly and compare non-`no-op` actions against explicit approval state;
- state verification should read the `values.root_module` resource tree and
  reject sensitive paths rather than leaking or comparing redacted values;
- cloud audit proof must retain principal, action, resource, time, region, and
  outcome because provider logs are the durable record of who did what to
  which cloud resource;
- provider-specific logs should be normalized into a strict JSONL artifact for
  deterministic local FSV, while the raw provider export remains the upstream
  source for production capture.

### Usage-Based Billing Meter Events

References:

- <https://docs.stripe.com/billing/subscriptions/usage-based/recording-usage-api>
- <https://docs.stripe.com/api/billing/meter-event/create>

Design implication:

- usage package rows need an event name, customer identifier, numeric value,
  timestamp, and idempotency identifier before they are suitable for billing
  analysis;
- usage events should be generated from immutable ledger rows so retries and
  rebuilds produce stable identifiers;
- Phase 15 writes usage-meter rows to JSONL package files only; it does not
  call a billing API from the CLI.

### NIST Privacy Framework And SP 800-53 Privacy/Audit Controls

References:

- <https://www.nist.gov/privacy-framework>
- <https://csrc.nist.gov/Pubs/sp/800/53/r5/upd1/Final>

Design implication:

- privacy review/deletion workflows need explicit policy, authorization, and
  durable audit evidence;
- audit records must be protected from unauthorized modification/deletion, so
  data subject erasure requests cannot silently remove append-only ledger rows;
- raw data-subject identifiers should be minimized in the ledger; stable
  digests are sufficient for equality/proof without persisting raw PII.

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
- Phase 15 reuses the same rule for `usage-meter-events.jsonl` and
  `eval-candidates.jsonl`; blank lines and invalid per-line JSON are package
  integrity failures.

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
- Phase 18 packages the demo report, evidence index, and script as PROV-like
  references to source ledger entities and verifier activities so the demo can
  be inspected without trusting the presenter.

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
- Phase 14 release gates preserve the evaluated input record references and
  source ledger tip so a downstream promotion can be tied back to the exact
  evidence state that was approved or blocked.
- Phase 15 package manifests preserve file hashes and input record refs so a
  buyer-facing report can be traced to the exact physical ledger state that
  produced it.
- Phase 18 reuses this artifact-verification discipline for the first-wedge
  demo: unrecognized/missing proof categories block package creation, and the
  output manifest is read back before the demo row is accepted.

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
- Phase 18 requires scanner absence and scanner-still-present demo evidence
  to come from persisted OSV JSON readbacks before the buyer demo package can
  pass.

### Executable FSV Certification And Provenance

References:

- <https://csrc.nist.gov/pubs/sp/800/218/final>
- <https://slsa.dev/spec/v1.2/provenance>
- <https://github.com/in-toto/attestation/tree/main/spec>
- <https://www.w3.org/TR/prov-dm/>
- <https://www.sqlite.org/pragma.html#pragma_integrity_check>

Design implication:

- FSV evidence should be durable product state, not only an external PR note;
- the certifier should use a provenance/attestation shape with subject
  digests, materials, builder identity, and invocation parameters;
- evidence packages must hash every file they emit and read those files back
  after promotion;
- database integrity needs an independent SQLite integrity check in addition
  to BlackBoxFix's hash-chain verification;
- the system must fail closed before appending `full_state_verification` when
  evidence post-state, deltas, edge cases, or artifact hashes do not match
  physical readback.

### Authenticated HTTP API Serving

References:

- <https://owasp.org/API-Security/editions/2023/en/0x11-t10/>
- <https://cheatsheetseries.owasp.org/cheatsheets/REST_Security_Cheat_Sheet.html>
- <https://docs.rs/axum/latest/axum/>
- <https://csrc.nist.gov/pubs/sp/800/218/final>

Design implication:

- API endpoints must perform authentication and tenant authorization at the
  endpoint boundary; tenant-scoped dashboard reads cannot reuse a global
  dashboard response without filtering physical rows first;
- request bodies need explicit resource limits and strict JSON schemas because
  unrestricted resource consumption and mass-assignment-style input are common
  API failure modes;
- management-style endpoints should default to loopback/private exposure and
  require explicit operator evidence before remote binds;
- errors should be typed and operationally useful without returning stack
  traces or raw secrets to clients;
- security-relevant events, including token validation failures, need durable
  audit rows so an operator can inspect the source of truth after the event;
- Rust HTTP serving should use a maintained routing/extractor framework over
  Tokio rather than a hand-rolled parser, keeping the ledger-specific logic in
  existing recorder functions.

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
